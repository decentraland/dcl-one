/** `0x1234…abcd`: an address short enough for a pill or a table cell. */
export const shortenAddress = (address: string) => `${address.slice(0, 6)}…${address.slice(-4)}`
