use itertools::Itertools;
use serenity::model::prelude::UserId;

use crate::{emoji::Emoji, emojis_with_counts::EmojisWithCounts};

/// A trade offer from one user to another user with an offered list of emojis and a requested list of emojis, both kept sorted.
///
/// This comes with methods for converting to and from the structure the database uses, and for outputting the emoji contents as text.
#[derive(PartialEq, Eq)]
pub(super) struct TradeOffer {
	offering_user: UserId,
	target_user: UserId,
	offer: EmojisWithCounts,
	request: EmojisWithCounts,
}

impl TradeOffer {
	/// Fails if the same emoji exists on both sides of the trade, but performs no other checks.
	pub fn new(
		user: UserId,
		target_user: UserId,
		offer: EmojisWithCounts,
		request: EmojisWithCounts,
	) -> Result<Self, String> {
		if offer.iter().any(|emoji| request.iter().contains(emoji)) {
			return Err(String::from("You put an emoji on both sides of the trade."));
		}
		Ok(Self {
			offering_user: user,
			target_user,
			offer,
			request,
		})
	}
	/// Unflattens the trade emoji information from the way the database has it.
	///
	/// Does no sanity checking as we trust the database.
	pub fn from_database(user: UserId, target_user: UserId, contents: Vec<(Emoji, i64)>) -> Self {
		let mut offer = Vec::new();
		let mut request = Vec::new();
		for (emoji, count) in contents {
			if count > 0 {
				request.push((emoji, count as u32));
			} else {
				offer.push((emoji, (-count) as u32));
			}
		}
		let offer = EmojisWithCounts::new(offer);
		let request = EmojisWithCounts::new(request);
		Self {
			offering_user: user,
			target_user,
			offer,
			request,
		}
	}
	pub fn new_recycling(user: UserId, offer: EmojisWithCounts) -> Self {
		let random_emoji = loop {
			let random_emoji = Emoji::random();
			if !offer.iter().any(|(emoji, _)| emoji == &random_emoji) {
				break random_emoji;
			}
		};
		Self {
			offering_user: user,
			target_user: UserId::new(0),
			offer,
			request: EmojisWithCounts::from_iter([(random_emoji, 1)]),
		}
	}
	/// Gets the first emoji in the request, which should be the only emoji if this is a recycling request.
	pub fn recycling_emoji(&self) -> Emoji {
		self.request.iter().next().unwrap().0
	}
	pub fn offering_user(&self) -> UserId {
		self.offering_user
	}
	pub fn target_user(&self) -> UserId {
		self.target_user
	}
	pub fn offer(&self) -> &EmojisWithCounts {
		&self.offer
	}
	pub fn request(&self) -> &EmojisWithCounts {
		&self.request
	}
	/// Generates a single list of emojis closer to the way the database stores it, with positive counts representing emojis the initiator will gain, and negative counts representing emojis the initiator will give away.
	pub fn to_database_format(&self) -> Vec<(Emoji, i64)> {
		self.request
			.iter()
			.map(|(emoji, count)| (*emoji, *count as i64))
			.chain(
				self.offer
					.iter()
					.map(|(emoji, count)| (*emoji, -(*count as i64))),
			)
			.collect()
	}
}
