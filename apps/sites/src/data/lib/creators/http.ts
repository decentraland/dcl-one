/** A response that arrived with a non-2xx status: the server's problem, unlike a request that never landed. */
export class HttpError extends Error {
  constructor(
    message: string,
    readonly status: number
  ) {
    super(`${message} (${status})`)
    this.name = 'HttpError'
  }
}
