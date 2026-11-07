module.exports = {
  moduleFileExtensions: ["ts", "js"],
  transform: {
    "^.+\\.ts$": ["ts-jest", { tsconfig: "test/tsconfig.json" }],
  },
  coveragePathIgnorePatterns: ["/node_modules/", "/test/", "/src/protocol/index.ts"],
  verbose: true,
  testMatch: ["**/*.spec.ts"],
  testEnvironment: "node",
}
