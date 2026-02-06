use agent_client_protocol as acp;
use tokio::sync::{mpsc, oneshot};
use tracing_subscriber::Layer;

pub struct SessionTracingLayer {
    session_id: acp::SessionId,
    update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
}

impl SessionTracingLayer {
    pub fn new(
        session_id: acp::SessionId,
        update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        Self {
            session_id,
            update_tx,
        }
    }

    fn send_message(&self, message: String) {
        let (tx, _rx) = oneshot::channel();
        let notification = acp::SessionNotification::new(
            self.session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(format!("{}\n\n", message)),
            ))),
        );

        self.update_tx.send((notification, tx)).ok();
    }
}

impl<S> Layer<S> for SessionTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanVisitor { fields: Vec::new() };
        attrs.record(&mut visitor);

        let span_name = attrs.metadata().name();
        let fields_str = if visitor.fields.is_empty() {
            String::new()
        } else {
            format!(" ({})", visitor.fields.join(", "))
        };

        self.send_message(format!("→ {}{}", span_name, fields_str));
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let target = metadata.target();

        if target.starts_with("reqwest") || target.starts_with("hyper") {
            return;
        }

        let mut visitor = EventVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        if !visitor.message.is_empty() {
            self.send_message(visitor.message);
        }
    }
}

struct SpanVisitor {
    fields: Vec<String>,
}

impl tracing::field::Visit for SpanVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={:?}", field.name(), value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.push(format!("{}={}", field.name(), value));
    }
}

struct EventVisitor {
    message: String,
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            if !self.message.is_empty() {
                self.message.push_str(", ");
            }
            self.message
                .push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            if !self.message.is_empty() {
                self.message.push_str(", ");
            }
            self.message
                .push_str(&format!("{}={}", field.name(), value));
        }
    }
}
