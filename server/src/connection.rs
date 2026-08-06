use crate::api::{ApiManager, FetchIsOnline};
use crate::client::UserMessage;

use bars_ipc::ConnectionTarget;

use actix::{
	Actor, ActorFutureExt, Addr, Arbiter, AsyncContext, Context, Handler, Message, MessageResult, WrapFuture,
};
use actix_broker::BrokerIssue;

#[derive(Message)]
#[rtype(result = "()")]
pub struct RequestConnection(pub ConnectionTarget);

#[derive(Message)]
#[rtype(result = "ConnectionTarget")]
pub struct GetConnection;

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct ConnectionChanged(pub ConnectionTarget);

pub struct ConnectionManager {
	api: Addr<ApiManager>,
	target: ConnectionTarget,
}

impl ConnectionManager {
	pub fn new(api: Addr<ApiManager>) -> Addr<Self> {
		let this = Self {
			api,
			target: ConnectionTarget::None,
		};
		Self::start_in_arbiter(&Arbiter::current(), |_ctx| this)
	}

	fn set_target(&mut self, target: ConnectionTarget) {
		if target != self.target {
			self.target = target;
			self.issue_system_async(ConnectionChanged(target));
		}
	}
}

impl Actor for ConnectionManager {
	type Context = Context<Self>;
}

impl Handler<GetConnection> for ConnectionManager {
	type Result = MessageResult<GetConnection>;

	fn handle(
		&mut self,
		_: GetConnection,
		_: &mut Self::Context,
	) -> Self::Result {
		MessageResult(self.target)
	}
}

impl Handler<RequestConnection> for ConnectionManager {
	type Result = ();

	fn handle(
		&mut self,
		RequestConnection(target): RequestConnection,
		ctx: &mut Self::Context,
	) {
		match target {
			ConnectionTarget::None => self.set_target(ConnectionTarget::None),
			ConnectionTarget::Network => {
				ctx.spawn(self.api.send(FetchIsOnline).into_actor(self).map(
					|res, this, _ctx| {
						let message = match res.map_err(|err| err.into()).flatten() {
							Ok(true) => {
								this.set_target(ConnectionTarget::Network);
								return
							},
							Ok(false) => "Failed to connect: not connected to VATSIM".into(),
							Err(err) => format!("Error checking network status: {err}"),
						};
						this.issue_system_async(UserMessage {
							aerodrome: None,
							message,
						});
					},
				));
			},
		}
	}
}
