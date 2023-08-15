use itertools::Itertools;
use serenity::{async_trait, model::prelude::*, prelude::*};
use sqlx::{Pool, Sqlite};

use crate::{daily_emoji::maybe_give_daily_emoji, emoji::EmojiMap, images, inventory, trading};

pub struct DiscordEventHandler {
	database: Pool<Sqlite>,
	emoji_map: EmojiMap,
}

impl DiscordEventHandler {
	pub fn new(database: Pool<Sqlite>, emoji_map: EmojiMap) -> Self {
		Self {
			database,
			emoji_map,
		}
	}
}

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn message(&self, context: Context, message: Message) {
		if !message.is_own(&context.cache) && !message.author.bot {
			maybe_give_daily_emoji(&self.database, context, message).await;
		}
	}

	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		if let Interaction::ApplicationCommand(interaction) = interaction {
			match interaction.data.name.as_str() {
				"inventory" => {
					inventory::command(&self.database, &self.emoji_map, context, interaction).await;
				}
				"image" => {
					images::command_make_raster_image(
						&self.database,
						&self.emoji_map,
						context,
						interaction,
					)
					.await;
				}
				"trade" => {
					trading::command::execute(
						&self.database,
						&self.emoji_map,
						context,
						interaction,
					)
					.await;
				}
				"generate" => {
					images::command_generate(&self.database, &self.emoji_map, context, interaction)
						.await;
				}
				"testimage" => {
					images::test_image(&self.emoji_map, context, interaction).await;
				}
				_ => (),
			};
		}
	}

	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		if let Some(arg) = arg {
			if &arg == "register" {
				for guild in context.cache.guilds() {
					let commands = guild
						.set_application_commands(&context.http, |commands| {
							commands
								.create_application_command(|command| inventory::register(command))
								.create_application_command(|command| {
									images::register_make_raster_image(command)
								})
								.create_application_command(|command| {
									trading::command::register(command)
								})
								.create_application_command(|command| {
									images::register_generate(command)
								})
								.create_application_command(|command| {
									images::register_test_image(command)
								})
						})
						.await
						.unwrap();

					let command_names = commands.into_iter().map(|command| command.name).join(", ");
					println!(
						"I now have the following guild slash commands in guild {}: {}",
						guild.as_u64(),
						command_names
					);
				}
			}
		}
	}
}
