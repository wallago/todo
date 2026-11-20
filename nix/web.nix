# The web app (apps/web), built to a folder of static files for
# todo-server's `[web] dir`.
{
  lib,
  stdenvNoCC,
  nodejs_24,
  pnpm,
  fetchPnpmDeps,
  pnpmConfigHook,
  src,
}:
stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "todo-web";
  inherit (lib.importJSON "${src}/package.json") version;
  inherit src;

  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    fetcherVersion = 4;
    hash = "sha256-6WKOYv/nJxa9QPDw0ztFF6ExyoLm76hihyB58n2E5B4=";
  };

  nativeBuildInputs = [
    nodejs_24
    pnpm
    pnpmConfigHook
  ];

  buildPhase = ''
    runHook preBuild
    pnpm exec vite build
    runHook postBuild
  '';

  # Run the vitest suite too: `nix flake check` builds this.
  doCheck = true;
  checkPhase = ''
    runHook preCheck
    pnpm exec vitest run
    runHook postCheck
  '';

  installPhase = ''
    runHook preInstall
    cp -r dist $out
    runHook postInstall
  '';
})
