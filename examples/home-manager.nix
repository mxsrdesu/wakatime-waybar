{ ... }:
{
  imports = [ ../nix/home-manager.nix ];
  programs.waybar-wakatime = {
    enable = true;
    bars = [ "main" ]; # use your own Waybar settings names
  };
  programs.waybar.settings.main.modules-center = [ "clock" "custom/pomodoro" "custom/wakatime" "custom/dnd" ];
}
