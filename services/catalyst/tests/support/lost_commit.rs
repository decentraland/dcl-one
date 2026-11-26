use sqlx::PgPool;

/// Drops the first armed COMMIT CommandComplete after PostgreSQL executed it.
/// All SQL and row locks are real; only the acknowledgment transport is faulted.
pub struct LostCommitProxy {
    pub pool: PgPool,
    armed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dropped_commits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl LostCommitProxy {
    pub async fn start(pool: &PgPool) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let options = pool.connect_options();
        let host = options.get_host().to_string();
        let port = options.get_port();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = listener.local_addr().unwrap().port();
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fault = armed.clone();
        let dropped_commits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dropped = dropped_commits.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept(), if connections.len()<4 => {
                        let (client,_) = accepted.unwrap();
                        client.set_nodelay(true).unwrap();
                        let host=host.clone(); let fault=fault.clone(); let dropped=dropped.clone();
                        connections.spawn(async move {
                            let server=tokio::net::TcpStream::connect((host.as_str(),port)).await.unwrap();
                            server.set_nodelay(true).unwrap();
                            let (mut client_read,mut client_write)=client.into_split();
                            let (mut server_read,mut server_write)=server.into_split();
                            let upstream=tokio::io::copy(&mut client_read,&mut server_write);
                            let downstream=async {
                                loop {
                                    let mut header=[0u8;5]; if server_read.read_exact(&mut header).await.is_err(){return;}
                                    let size=u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;
                                    assert!((4..=256*1024).contains(&size)); let mut body=vec![0;size-4];
                                    if server_read.read_exact(&mut body).await.is_err(){return;}
                                    if header[0]==b'C' && body==b"COMMIT\0" && fault.swap(false,std::sync::atomic::Ordering::SeqCst){
                                        dropped.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                                        return;
                                    }
                                    // Forward a complete response frame without a header/body delay.
                                    let mut frame=Vec::with_capacity(header.len()+body.len());
                                    frame.extend_from_slice(&header);
                                    frame.extend_from_slice(&body);
                                    if client_write.write_all(&frame).await.is_err(){return;}
                                }
                            };
                            tokio::select! { _=upstream=>{}, _=downstream=>{} }
                        });
                    }
                    _=connections.join_next(), if !connections.is_empty()=>{}
                }
            }
        });
        let redirected = options
            .as_ref()
            .clone()
            .host("127.0.0.1")
            .port(proxy_port)
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(redirected)
            .await
            .unwrap();
        Self {
            pool,
            armed,
            dropped_commits,
            task,
        }
    }
    pub fn dropped_commits(&self) -> usize {
        self.dropped_commits
            .load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn lose_next_commit(&self) {
        self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
impl Drop for LostCommitProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
