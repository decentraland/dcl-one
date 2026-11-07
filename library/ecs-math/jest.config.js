module.exports = {
  moduleFileExtensions: ["ts", "js"],
  transform: {
    "^.+\\.ts$": ["ts-jest", { tsconfig: "test/tsconfig.json" }],
  },
  verbose: true,
  testMatch: ["**/*.spec.ts"],
  testEnvironment: "node",
}
