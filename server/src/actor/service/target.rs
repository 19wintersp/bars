use crate::actor::client::DispatchUserMessage;
use crate::actor::service::api::{ApiService, FetchIsOnline};

use bars_ipc::ConnectionTarget;

use actix::{
	Actor, ActorFutureExt, AsyncContext, Context, Handler, Message,
	MessageResult, Supervised, SystemService, WrapFuture,
};
use actix_broker::BrokerIssue;

#[derive(Message)]
#[rtype(result = "ConnectionTarget")]
pub struct GetTarget;

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct SetTarget(pub ConnectionTarget);

pub struct TargetService {
	target: ConnectionTarget,
}

impl TargetService {
	fn set_target(&mut self, target: ConnectionTarget) {
		if target != self.target {
			self.target = target;
			self.issue_system_async(SetTarget(target));
		}
	}
}

impl Default for TargetService {
	fn default() -> Self {
		Self {
			target: ConnectionTarget::None,
		}
	}
}

impl Actor for TargetService {
	type Context = Context<Self>;
}

impl Supervised for TargetService {}

impl SystemService for TargetService {}

impl Handler<GetTarget> for TargetService {
	type Result = MessageResult<GetTarget>;

	fn handle(&mut self, _: GetTarget, _ctx: &mut Self::Context) -> Self::Result {
		MessageResult(self.target)
	}
}

impl Handler<SetTarget> for TargetService {
	type Result = ();

	fn handle(&mut self, SetTarget(target): SetTarget, ctx: &mut Self::Context) {
		match target {
			ConnectionTarget::None | ConnectionTarget::Local => {
				self.set_target(target);
			},
			ConnectionTarget::Network => {
				ctx.spawn(
					ApiService::from_registry()
						.send(FetchIsOnline)
						.into_actor(self)
						.map(|res, this, _ctx| {
							let message = match res.map_err(|err| err.into()).flatten() {
								Ok(true) => {
									this.set_target(ConnectionTarget::Network);
									return
								},
								Ok(false) => {
									"Failed to connect: not connected to VATSIM".into()
								},
								Err(err) => format!("Error checking network status: {err}"),
							};
							this.issue_system_async(DispatchUserMessage {
								aerodrome: None,
								message,
							});
						}),
				);
			},
		}
	}
}
