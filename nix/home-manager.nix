{ config, lib, pkgs, ... }:
let
  cfg = config.programs.waybar-wakatime;
  example = (builtins.fromJSON (builtins.readFile ../examples/waybar.jsonc))."custom/wakatime";
  settings = example // {
    exec = "${cfg.package}/bin/waybar-wakatime status";
    menu-file = "${cfg.package}/share/waybar-wakatime/menu.xml";
    menu-actions = {
      dashboard = "${cfg.package}/bin/waybar-wakatime dashboard";
      refresh = "${cfg.package}/bin/waybar-wakatime refresh";
      settings = "${cfg.package}/bin/waybar-wakatime settings";
    };
  };
in {
  options.programs.waybar-wakatime = {
    enable = lib.mkEnableOption "wakatime daily total for waybar";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ../package.nix {};
      description = "the waybar-wakatime package";
    };
    bars = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = "waybar settings names; set module order and styling separately";
    };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
    programs.waybar.settings = lib.genAttrs cfg.bars (_: {
      "custom/wakatime" = lib.mapAttrsRecursive (_: lib.mkDefault) settings;
    });
  };
}
