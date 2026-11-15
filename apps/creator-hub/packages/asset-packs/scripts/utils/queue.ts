type Task = () => Promise<unknown>;
type Waiter = { resolve: () => void; reject: (error: unknown) => void };

/**
 * Runs tasks with a limit on how many run at the same time. After the first task that
 * fails, the queue starts no more tasks and `onIdle` rejects with the error of that task.
 */
export class Queue {
  private readonly concurrency: number;
  private readonly tasks: Task[] = [];
  private readonly waiters: Waiter[] = [];
  private running = 0;
  private failure: { error: unknown } | undefined;

  constructor(concurrency: number) {
    this.concurrency = concurrency;
  }

  add(task: Task): void {
    this.tasks.push(task);
    this.next();
  }

  /** Resolves when no task runs and no task waits. Rejects when a task failed. */
  onIdle(): Promise<void> {
    if (this.failure) return Promise.reject(this.failure.error);
    if (this.running === 0 && this.tasks.length === 0) return Promise.resolve();
    return new Promise((resolve, reject) => this.waiters.push({ resolve, reject }));
  }

  private next(): void {
    while (!this.failure && this.running < this.concurrency) {
      const task = this.tasks.shift();
      if (!task) return;
      this.running++;
      Promise.resolve()
        .then(task)
        .then(
          () => this.settle(),
          error => this.settle({ error }),
        );
    }
  }

  private settle(failure?: { error: unknown }): void {
    this.running--;
    this.failure ??= failure;
    if (this.failure) {
      for (const waiter of this.waiters.splice(0)) waiter.reject(this.failure.error);
      return;
    }
    this.next();
    if (this.running === 0 && this.tasks.length === 0) {
      for (const waiter of this.waiters.splice(0)) waiter.resolve();
    }
  }
}
