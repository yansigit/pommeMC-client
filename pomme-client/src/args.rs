use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "pomme", about = "Minecraft client")]
pub struct LaunchArgs {
    #[arg(long)]
    pub version: Option<String>,

    #[arg(long)]
    pub username: Option<String>,

    #[arg(long)]
    pub uuid: Option<String>,

    #[arg(long)]
    pub access_token: Option<String>,

    #[arg(long)]
    pub launch_token: Option<String>,

    #[arg(long)]
    pub assets_dir: Option<String>,

    #[arg(long)]
    pub versions_dir: Option<String>,

    #[arg(long)]
    pub game_dir: Option<String>,

    #[arg(long)]
    pub quick_access_multiplayer: Option<String>,

    /// Start a fresh integrated singleplayer world without menu input.
    #[arg(long, conflicts_with = "quick_access_multiplayer")]
    pub quick_access_singleplayer: bool,

    /// Capture the first rendered frame to the normal screenshots directory.
    /// Useful for automated renderer smoke tests without synthetic key input.
    #[arg(long)]
    pub capture_first_frame: bool,

    /// Capture the first fully loaded gameplay frame after joining a server.
    #[arg(long)]
    pub capture_gameplay_frame: bool,

    /// Place all four copper golem statue poses in the automated integrated
    /// world before capturing gameplay.
    #[arg(
        long,
        requires = "quick_access_singleplayer",
        requires = "capture_gameplay_frame"
    )]
    pub showcase_copper_golem_statues: bool,

    /// Exit after the requested screenshot has been written.
    #[arg(long)]
    pub exit_after_screenshot: bool,

    /// Run a checked-in non-GUI action script through the normal input
    /// handlers.
    #[arg(long, requires = "automation_trace")]
    pub automation_script: Option<String>,

    /// Write the action script's event and assertion trace as JSON.
    #[arg(long, requires = "automation_script")]
    pub automation_trace: Option<String>,
}
