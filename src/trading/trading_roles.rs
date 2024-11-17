use serenity::all::{GuildId, RoleId, UserId};

use crate::context::Context;

pub(crate) fn get_trading_roles() -> Vec<RoleId> {
	std::fs::read_to_string("./roles.txt")
		.expect("Could not read roles file.")
		.lines()
		.map(|line| line.parse().map(RoleId::new))
		.collect::<Result<_, _>>()
		.expect("Could not parse roles file.")
}

pub(super) async fn has_trading_role(context: Context<'_>, guild: GuildId, user: UserId) -> bool {
	guild
		.member(context, user)
		.await
		.map(|member| {
			member
				.roles
				.iter()
				.any(|role| context.trading_roles.contains(role))
		})
		.unwrap_or(false)
}
