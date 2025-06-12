use rspotify::model::{Device, PlayContextId, PlaylistId};
use rspotify::{AuthCodeSpotify, Config, Credentials, OAuth, prelude::OAuthClient, scopes};

async fn with_auth(creds: Credentials, oauth: OAuth, config: Config) {
    // In the first session of the application we authenticate and obtain the
    // refresh token.
    println!(">>> Session one, obtaining refresh token and running some requests:");
    let spotify = AuthCodeSpotify::with_config(creds.clone(), oauth, config.clone());
    let url = spotify.get_authorize_url(false).unwrap();
    // NOTE: This function requires the `cli` feature enabled.
    spotify
        .prompt_for_token(&url)
        .await
        .expect("couldn't authenticate successfully");

    let devices = spotify.device().await.unwrap();
    let device = {
        let mut _device: Option<Device> = None;
        for device in devices {
            println!("{}", device.name);
            if device.name == "Christian's Echo Spot" {
                _device = Some(device);
            }
        }
        if let Some(d) = _device {
            d
        } else {
            eprintln!("Could not find the device spedified.");
            return;
        }
    };
    println!("{}", device.name);
    spotify
        .shuffle(true, Some(device.id.clone().unwrap().as_str()))
        .await
        .unwrap();
    let result = spotify
        .start_context_playback(
            PlayContextId::Playlist(unsafe {
                PlaylistId::from_id_unchecked("37i9dQZF1DWUZ5bk6qqDSy")
            }),
            Some(device.id.clone().unwrap().as_str()),
            None,
            None,
        )
        .await;

    match result {
        Ok(_) => println!("Playback started!"),
        Err(e) => {
            eprintln!("Playback failed: {e}");
            if let rspotify::ClientError::Http(resp) = &e {
                eprintln!("Details: {}", resp);
            }
        }
    }
}

pub async fn run_spotify() {
    // Enabling automatic token refreshing in the config
    let config = Config {
        token_cached: true,
        ..Default::default()
    };

    let creds = Credentials::from_env().unwrap();
    let oauth = OAuth::from_env(scopes!(
        "user-follow-read",
        "user-follow-modify",
        "user-read-playback-state",
        "user-modify-playback-state"
    ))
    .unwrap();

    with_auth(creds.clone(), oauth, config.clone()).await;
}
