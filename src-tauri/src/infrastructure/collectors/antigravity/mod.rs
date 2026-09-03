mod adapter;
mod app_ide_sqlite_reader;
mod cli_sqlite_reader;
mod conversation_index;
mod discovery;
mod mapper;
pub(crate) mod product_variant;
mod protobuf_usage;
mod runtime_client;
mod runtime_metadata_client;
mod usage_cache;
mod usage_extractor;

const PROFILE_VERSION: u16 = 3;

pub(crate) use adapter::AntigravityCollector;
pub(crate) use conversation_index::{ConversationDatabase, ConversationIndex};
pub(crate) use discovery::{RuntimeDiscovery, RuntimeDiscoveryReport, RuntimeEndpoint};
pub(crate) use runtime_client::RuntimeClient;
pub(crate) use usage_extractor::{extract_usage_from_generator_metadata, AntigravityUsageRecord};
