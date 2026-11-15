/** A promise that the holder settles from outside. */
export type IFuture<T> = Promise<T> & {
  resolve: (value: T) => void;
  reject: (error: Error) => void;
};

export function future<T = void>(): IFuture<T> {
  let settle!: Pick<IFuture<T>, 'resolve' | 'reject'>;
  const promise = new Promise<T>((resolve, reject) => {
    settle = { resolve, reject };
  });
  return Object.assign(promise, settle);
}
