use chrono::Utc;
use discord_presence::Client;
//use discord_rich_presence::activity::{Activity, Assets, Timestamps};
//use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::thread;
use std::time::Duration;

use crate::consts::{/*FAKEYOU_DISCORD_APP_ID,*/ FAKEYOU_DISCORD_APP_ID_INT};

#[derive(Clone, Debug)]
pub enum DiscordError {
    //ConfigError,
    ConnectionError,
    StatusUpdateError,
}

pub fn spawn_discord_thread() {
    let launch_timestamp = Utc::now().timestamp().unsigned_abs();
    thread::spawn(move || {
        loop {
            let result = launch_discord_presence_alt(launch_timestamp);
            let wait_seconds = match result {
                Ok(_) => 60,
                Err(_e) => 5,
            };
            thread::sleep(Duration::from_secs(wait_seconds));
        }
    });
}

/*pub fn launch_discord_presence() -> Result<(), DiscordError> {
    // NB: We go to a lot of trouble error handling here because the Discord crate authors
    // use dyn-sized errors that are not at all ergonomic.
    let mut client = DiscordIpcClient::new(FAKEYOU_DISCORD_APP_ID)
        .map_err(|_e| DiscordError::ConfigError)?;

    client.connect()
        .map_err(|_e| DiscordError::ConnectionError)?;

    client.set_activity(Activity::new()
        .state("Audio Shenanigans @ FakeYou.com")
        .details("[hidden voice]")
        .assets(Assets::new()
            .large_image("mask")
            .large_text("FakeYou"))
        .timestamps(Timestamps::new()
            .start(Utc::now().timestamp())
        )
    ).map_err(|_e| DiscordError::StatusUpdateError)?;

    Ok(())
}*/

pub fn launch_discord_presence_alt(launch_timestamp: u64) -> Result<(), DiscordError> {
    let mut client = Client::new(FAKEYOU_DISCORD_APP_ID_INT);

    let join_handle = client.start();

    client.set_activity(|activity| {
        activity.state("Audio Shenanigans @ FakeYou.com")
            .details("[Hidden] Voice")
            .assets(|assets| {
                assets.large_image("mask")
                    .large_text("FakeYou")
            })
            .timestamps(|timestamps| {
                timestamps.start(launch_timestamp)
            })
    }).map_err(|_e| DiscordError::StatusUpdateError)?;

    join_handle.join()
        .map_err(|_e| DiscordError::ConnectionError)?;

    Ok(())
}
