{
  description = "canbus-gauge dev environment";

  inputs = {
    nixpkgs-default.url = "github:nixos/nixpkgs/nixos-26.05";
    nixpkgs-latest.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs-default, nixpkgs-latest }:
    let
      systems = nixpkgs-default.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
    in {
      devShells = systems (system:
        let
          pkgs = import nixpkgs-default { inherit system; };
          pkgs-latest = import nixpkgs-latest { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = [
              # Note: Rust toolchain is managed by rustup instead of by nix
              # for the sake of rover compatibility and.. general sanity.
              #
              # This means that since the toolchain is not managed by nix, a
              # `nix-store --gc` might remove the glibc that the rustup-managed
              # toolchain linked against. In that case (which usually manifests
              # as cargo not found), run:
              #
              #   `rustup toolchain uninstall [current verison]`
              #   `rustup toolchain install`
              #
              pkgs.rustup

              # ARM debug utils
              pkgs.cargo-binutils
              pkgs.flip-link
              pkgs.gcc-arm-embedded
              pkgs.pkg-config
              pkgs.probe-rs-tools

              # CAD, EDA, and misc utils
              #
              # Note: FreeCAD and KiCad store a lot of global config in user's
              # home dir. It's possible to create wrappers and so on such that
              # this flake maintains its own totally separate version of those
              # packages and their associated configs. But that sounds like a
              # pain so I just use my system versions for the time being :)
              pkgs.can-utils
              #pkgs-latest.freecad
              #pkgs.freerouting
              #pkgs.kicad
              #(pkgs.python3.withPackages (ps: [ ps.kicad ]))
            ];

            buildInputs = [
              pkgs.SDL2 # Required for graphics simulator
            ];
          };
        });
      };
}
