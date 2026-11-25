// The item's script: the build imports it for the Script component of entity
// 512 and the scene starts it before main().
export class Counter {
  constructor(
    private src: string,
    private entity: number
  ) {}

  start() {
    console.log(`Counter script started: src=${this.src} entity=${this.entity}`)
  }
}
