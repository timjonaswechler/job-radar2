// Chat and session contracts exercise the native durable-storage adapter. Windows
// durability remains owned by #294; keep only provider-neutral conversation
// contracts portable until that implementation and its native acceptance exist.
#[cfg(unix)]
mod chats;
mod conversation;
#[cfg(unix)]
mod sessions;
