"""Fixed public dummy observations and typed verify-only profile, not credentials."""
import hashlib
import json

VECTORS = {
    "empty": "$wp$2y$04$......................95Tr8E4rK4vqpGi1aEzSvD8w4ihtXuy",
    "dummy": "$wp$2y$04$......................p4I2i5y/qZ1mBr4uEKqha7UHXK0Clmq",
    "binary": "$wp$2y$04$......................S3bG01U.z0mtO6foDhFLmZTx5V6GN5K",
    "spaced": "$wp$2y$04$......................hl8fSCoR6uBYSLfV/VnjGvO8VRxZaWa",
    "max4096": "$wp$2y$04$......................PJ0SHzxZ5XVAIaQ.o5XGE7.jlbWF7K2",
    "cost12": "$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C",
}
VECTORS_SHA256 = "6b9ff298abe122a1a9603bf9ba531f7cd460a3bd6e80e9ee35e47aaa0150b586"
PROFILE = {"php_version": "8.5.12", "password_type": "bytes", "hash_type": "bytes",
           "hash_format": "$wp$2y$", "hash_bytes": 63, "password_max_bytes": 4096,
           "prehash": "HMAC-SHA384/base64_standard", "custom_hasher": "empty", "filter_registry": "empty",
           "hook": "check_password", "user_id": 0, "native_filter_bookkeeping": False,
           "host_max_cost": "required_4_to_31", "source_sha256": "3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777",
           "bcrypt_version": "0.19.3", "hmac_version": "0.12.1", "sha2_version": "0.10.9",
           "base64_version": "0.22.1", "zeroize_version": "1.9.1", "dummy_vectors_sha256": VECTORS_SHA256}

def vectors_hash():
    return hashlib.sha256(json.dumps(list(map(list, VECTORS.items())), separators=(",", ":")).encode("ascii")).hexdigest()
