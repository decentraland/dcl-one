// /robots.txt, as decentraland/sites serves public/robots.txt: every crawler may fetch every path.
// Its Sitemap line is left out: this site serves no /sitemap.xml.
const ROBOTS_TXT = "User-agent: *\nAllow: /\n";

export function loader() {
  return new Response(ROBOTS_TXT, {
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Cache-Control": "public, max-age=3600",
    },
  });
}
