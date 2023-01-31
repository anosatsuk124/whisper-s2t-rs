use serenity::framework::standard::Args;
use serenity::framework::standard::{macros::command, CommandResult};
use serenity::model::prelude::*;
use serenity::{async_trait, prelude::*};
use songbird::model::payload::{ClientDisconnect, Speaking};
use songbird::{CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler};

fn check_msg<T: std::fmt::Debug>(result: Result<Message, T>) {
    match result {
        Ok(success) => println!("Sending message: {:?}", success),
        Err(why) => eprintln!("Error sending message: {:?}", why),
    }
}

struct Receiver;

impl Receiver {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        use EventContext as Ctx;
        match ctx {
            Ctx::SpeakingStateUpdate(Speaking {
                speaking,
                ssrc,
                user_id,
                ..
            }) => {
                println!(
                    "Speaking state update: user {:?} has SSRC {:?}, using {:?}",
                    user_id, ssrc, speaking,
                );
            }
            Ctx::SpeakingUpdate(data) => {
                println!(
                    "Source {} has {} speaking.",
                    data.ssrc,
                    if data.speaking { "started" } else { "stopped" },
                );
            }
            Ctx::VoicePacket(data) => {
                if let Some(audio) = data.audio {
                    println!(
                        "Audio packet's first 5 samples: {:?}",
                        audio.get(..5.min(audio.len()))
                    );
                    println!(
                        "Audio packet sequence {:05} has {:04} bytes (decompressed from {}), SSRC {}",
                        data.packet.sequence.0,
                        audio.len() * std::mem::size_of::<i16>(),
                        data.packet.payload.len(),
                        data.packet.ssrc,
                    );
                } else {
                    println!("RTP packet, but no audio. Driver may not be configured to decode.");
                }
            }
            Ctx::RtcpPacket(data) => {
                println!("RTCP packet received: {:?}", data.packet);
            }
            Ctx::ClientDisconnect(ClientDisconnect { user_id, .. }) => {
                println!("Client disconnected: user {:?}", user_id);
            }
            _ => {
                // We won't be registering this struct for any more event classes.
                unimplemented!()
            }
        }

        None
    }
}

#[command]
#[description = "Join the voice channel"]
pub async fn join(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let connect_to = match args.single::<u64>() {
        Ok(id) => ChannelId(id),
        Err(_) => {
            check_msg(
                msg.reply(ctx, "Requires a valid voice channel ID be given")
                    .await,
            );

            return Ok(());
        }
    };

    let songbird = songbird::serenity::get(ctx)
        .await
        .expect("Failed to initialize the Songbird Client");
    if let Some(guild) = msg.guild(&ctx.cache) {
        let guild_id = guild.id;
        let (handler_lock, conn_result) = songbird.join(guild_id, connect_to).await;
        if conn_result.is_ok() {
            let mut handler = handler_lock.lock().await;

            handler.add_global_event(CoreEvent::SpeakingStateUpdate.into(), Receiver::new());

            handler.add_global_event(CoreEvent::SpeakingUpdate.into(), Receiver::new());

            handler.add_global_event(CoreEvent::VoicePacket.into(), Receiver::new());

            handler.add_global_event(CoreEvent::RtcpPacket.into(), Receiver::new());

            handler.add_global_event(CoreEvent::ClientDisconnect.into(), Receiver::new());

            check_msg(
                msg.channel_id
                    .say(&ctx.http, &format!("Joined {}", connect_to.mention()))
                    .await,
            );
        } else {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Error joining the channel")
                    .await,
            );
        }
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}
