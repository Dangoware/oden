use std::{borrow::BorrowMut, process::{Child, Command, Stdio}};

use serenity::async_trait;
use songbird::{input::{ChildContainer, YoutubeDl}, Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use url::Url;

use crate::{Context, CurrentTrack, Error, HttpKey};

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
    input_query: Vec<String>
) -> Result<(), Error> {
    let input_query = match input_query.is_empty() {
        false => input_query,
        true => {
            ctx.channel_id()
                .say(&ctx.http(), "Must provide a URL to a video or audio")
                .await?;

            return Ok(());
        },
    };

    let no_search = input_query[0].starts_with("http");

    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let http_client = {
            let data = ctx.serenity_context().data.read().await;
            data.get::<HttpKey>()
                .cloned()
                .expect("Guaranteed to exist in the typemap.")
        };

        let yt_dlp = if no_search {
            YoutubeDl::new(http_client, input_query[0].clone())
        } else {
            YoutubeDl::new_search(http_client, input_query.join(" "))
        };

        let track = handler.play_input(yt_dlp.into());

        let mut data = ctx.serenity_context().data.write().await;
        data.insert::<CurrentTrack>(track);

        ctx.channel_id().say(&ctx.http(), "Playing song").await?;
    } else {
        ctx.channel_id()
            .say(&ctx.http(), "Not in a voice channel to play in")
            .await?;
    }

    Ok(())
}

#[poise::command(prefix_command, track_edits)]
pub async fn stop(
    ctx: Context<'_>,
    input_query: Vec<String>
) -> Result<(), Error> {
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let guild_id = ctx.guild_id().unwrap();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        handler.stop();

        let mut data = ctx.serenity_context().data.write().await;
        data.remove::<CurrentTrack>();
    }

    Ok(())
}
