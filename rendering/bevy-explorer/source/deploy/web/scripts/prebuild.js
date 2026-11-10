const fs = require("fs");

let ENV_CONTENT = {};

const packageJson = JSON.parse(fs.readFileSync("./package.json").toString());

Object.assign(ENV_CONTENT, getPublicUrls());

packageJson.homepage = ENV_CONTENT["PUBLIC_URL"];

if (packageJson.homepage) {
  if (process.env.GITHUB_OUTPUT) {
    fs.appendFileSync(process.env.GITHUB_OUTPUT,
      `public_url=${packageJson.homepage}\npublic_path=${new URL(packageJson.homepage).pathname}\n`);
  }
}

console.log("VERSIONS: ", Object.entries(ENV_CONTENT), "\n");

fs.writeFileSync(
  ".env",
  Object.entries(ENV_CONTENT)
    .map((e) => e[0] + "=" + JSON.stringify(e[1]))
    .join("\n") + "\n"
);

fs.writeFileSync("./package.json", JSON.stringify(packageJson, null, 2));

// No HTML rewriting: the React app (the only page) bakes PUBLIC_URL in at build time (vite base),
// and sets window.PUBLIC_URL for the engine's pkg/ fetches (react-web EngineHost). This script's
// remaining job is stamping homepage/.env and the output environment file the CDN deployer reads.

function getPublicUrls() {
  if (!process.env.GEN_STATIC_LOCAL) {
    if (process.env.CI) {
      return {
        PUBLIC_URL: `https://cdn.decentraland.org/${packageJson.name}/${packageJson.version}`,
      };
    }
  }

  return {
    PUBLIC_URL: ``,
  };
}
