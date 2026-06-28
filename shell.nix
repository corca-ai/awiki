{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [ pkgs.cargo pkgs.rustc pkgs.rustfmt pkgs.clippy ];
}
