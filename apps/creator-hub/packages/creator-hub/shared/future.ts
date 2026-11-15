/** A promise that the holder settles from outside, and that tells if it is settled. */
export type IFuture<T> = Promise<T> & {
  resolve: (value: T) => void;
  reject: (error: Error) => void;
  isPending: boolean;
};

export function future<T = void>(): IFuture<T> {
  let settle!: { resolve: (value: T) => void; reject: (error: Error) => void };
  const promise = new Promise<T>((resolve, reject) => {
    settle = { resolve, reject };
  });
  const result: IFuture<T> = Object.assign(promise, {
    isPending: true,
    resolve: (value: T) => {
      settle.resolve(value);
      result.isPending = false;
    },
    reject: (error: Error) => {
      settle.reject(error);
      result.isPending = false;
    },
  });
  return result;
}
