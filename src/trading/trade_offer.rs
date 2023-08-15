use itertools::Itertools;
use serenity::model::prelude::UserId;
use std::fmt::Write;

use crate::emoji::Emoji;

/// A trade offer from one user to another user with an offered list of emojis and a requested list of emojis, both kept sorted.
///
/// This comes with methods for converting to and from the structure the database uses, and for outputting the emoji contents as text.
#[derive(PartialEq, Eq)]
pub(super) struct TradeOffer {
	offering_user: UserId,
	target_user: UserId,
	offer: Vec<(Emoji, i64)>,
	request: Vec<(Emoji, i64)>,
}

impl TradeOffer {
	/// Fails if the same emoji exists on both sides of the trade, but performs no other checks.
	pub fn new(
		user: UserId,
		target_user: UserId,
		mut offer: Vec<(Emoji, i64)>,
		mut request: Vec<(Emoji, i64)>,
	) -> Result<Self, String> {
		if offer.iter().any(|emoji| request.iter().contains(emoji)) {
			return Err(String::from("You put an emoji on both sides of the trade."));
		}
		offer.sort_by_key(|(emoji, _)| *emoji);
		request.sort_by_key(|(emoji, _)| *emoji);
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
				request.push((emoji, count));
			} else {
				offer.push((emoji, -count));
			}
		}
		offer.sort_by_key(|(emoji, _)| *emoji);
		request.sort_by_key(|(emoji, _)| *emoji);
		Self {
			offering_user: user,
			target_user,
			offer,
			request,
		}
	}
	pub fn offering_user(&self) -> UserId {
		self.offering_user
	}
	pub fn target_user(&self) -> UserId {
		self.target_user
	}
	pub fn offer(&self) -> &Vec<(Emoji, i64)> {
		&self.offer
	}
	pub fn request(&self) -> &Vec<(Emoji, i64)> {
		&self.request
	}
	pub fn write_offer<T: Write>(&self, mut buffer: T) {
		for (emoji, count) in &self.offer {
			for _ in 0..*count {
				write!(buffer, "{}", emoji.as_str()).unwrap();
			}
		}
	}
	pub fn write_request<T: Write>(&self, mut buffer: T) {
		for (emoji, count) in &self.request {
			for _ in 0..*count {
				write!(buffer, "{}", emoji.as_str()).unwrap();
			}
		}
	}
	/// Generates a single list of emojis closer to the way the database stores it, with positive counts representing emojis the initiator will gain, and negative counts representing emojis the initiator will give away.
	pub fn flatten(&self) -> Vec<(Emoji, i64)> {
		self.request
			.iter()
			.copied()
			.chain(self.offer.iter().map(|(emoji, count)| (*emoji, -count)))
			.collect()
	}
}
