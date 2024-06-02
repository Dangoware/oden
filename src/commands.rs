use std::process::{Child, Command, Stdio};

use serenity::async_trait;
use songbird::{input::ChildContainer, Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use url::Url;

use crate::{Context, Error};

struct TrackErrorNotifier;

#[async_trait]
impl VoiceEventHandler for TrackErrorNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                println!(
                    "Track {:?} encountered an error: {:?}",
                    handle.uuid(),
                    state.playing
                );
            }
        }

        None
    }
}

/// Show this help menu
#[poise::command(prefix_command, track_edits)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "This is not a bot",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

#[poise::command(prefix_command, track_edits)]
pub async fn join(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let (guild_id, channel_id) = {
        let guild = ctx.guild().unwrap();
        let channel_id = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            //check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Ok(handler_lock) = manager.join(guild_id, connect_to).await {
        // Attach an event handler to see notifications of all track errors.
        let mut handler = handler_lock.lock().await;
        handler.add_global_event(TrackEvent::Error.into(), TrackErrorNotifier);
    }

    Ok(())
}

#[poise::command(prefix_command, track_edits)]
pub async fn leave(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        manager.remove(guild_id).await?;

        ctx.channel_id().say(&ctx.http(), "Left voice channel").await?;
    } else {
        ctx.reply("Not in a voice channel").await?;
    }

    Ok(())
}

#[poise::command(prefix_command, track_edits)]
pub async fn play(
    ctx: Context<'_>,
    url: Option<Url>
) -> Result<(), Error> {
    let url = match url {
        Some(url) => url,
        None => {
            ctx.channel_id()
                .say(&ctx.http(), "Must provide a URL to a video or audio")
                .await?;

            return Ok(());
        },
    };

    let do_search = !url.as_str().starts_with("http");

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let yt_dlp = Command::new("yt-dlp")
            .args(vec![
                "-f", "bestaudio",
                "-o", "-",
                url.as_str()
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let src = ChildContainer::new(
            vec![
                yt_dlp,
            ]
        );
        let _ = handler.play_input(src.into());

        ctx.channel_id().say(&ctx.http(), "Playing song").await?;
    } else {
        ctx.channel_id()
            .say(&ctx.http(), "Not in a voice channel to play in")
            .await?;
    }

    Ok(())
}
