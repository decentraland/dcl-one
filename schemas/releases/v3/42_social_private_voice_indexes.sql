-- Shared by fresh bootstrap and the atomic version-3 release. The release
-- creates these access paths before dropping the former address UNIQUEs.
CREATE INDEX private_voice_chats_caller_idx ON social.private_voice_chats (caller_address);
CREATE INDEX private_voice_chats_callee_idx ON social.private_voice_chats (callee_address);
