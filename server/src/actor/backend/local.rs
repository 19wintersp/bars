use crate::actor::backend::{
	Backend, BackendDownstream, BackendUpstream, ConnectionState,
};

use bars_graph::Patch;
use bars_protocol::{ConnectionType, Downstream, Upstream};

use actix::{Actor, Addr, Context, Handler, WeakAddr};
use tracing::{debug, warn};

pub struct Local {
	backend: WeakAddr<Backend>,
}

impl Local {
	pub fn new(backend: WeakAddr<Backend>) -> Addr<Self> {
		Self::create(move |_ctx| Self { backend })
	}

	fn send(&self, message: Downstream<Option<Patch>>) {
		self
			.backend
			.upgrade()
			.map(|backend| backend.do_send(BackendDownstream(message)));
	}
}

impl Actor for Local {
	type Context = Context<Self>;

	fn started(&mut self, _ctx: &mut Self::Context) {
		debug!("started");

		self.backend.upgrade().map(|backend| {
			backend.do_send(ConnectionState::Connected);
			backend.do_send(BackendDownstream(Downstream::InitialState {
				connection_type: ConnectionType::Controller,
				scenery: Vec::new(),
				patch: None,
			}));
		});
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		debug!("stopped");
	}
}

impl Handler<BackendUpstream> for Local {
	type Result = ();

	fn handle(
		&mut self,
		BackendUpstream(message): BackendUpstream,
		_ctx: &mut Self::Context,
	) {
		match message {
			Upstream::Heartbeat => self.send(Downstream::HeartbeatAck),
			Upstream::HeartbeatAck => (),
			Upstream::Close => {
				self.send(Downstream::Close);
				self
					.backend
					.upgrade()
					.map(|backend| backend.do_send(ConnectionState::Error));
			},
			Upstream::GetState => warn!("unhandled packet"),
			Upstream::StateUpdate { .. } | Upstream::MultiStateUpdate { .. } => (),
			Upstream::SharedStateUpdate { patch } => {
				self.send(Downstream::SharedStateUpdate {
					patch: Some(patch),
					controller_id: "local".into(),
				});
			},
			Upstream::GetOnlinePilots => {
				self.send(Downstream::OnlinePilots { pilots: Vec::new() });
			},
		}
	}
}
