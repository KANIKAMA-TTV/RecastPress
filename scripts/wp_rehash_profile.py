"""Explicit metadata-only default profile, separate from verification/generation policy."""
PROFILE = {'php_version': '8.5.12', 'hash_type': 'bytes', 'custom_hasher': 'empty',
           'filter_registry': 'empty', 'user_id': 0, 'algorithm': '2y', 'options': 'empty',
           'default_cost': 12, 'default_cost_confirmed': True,
           'nonprefixed': 'arbitrary_bytes_true', 'prefixed': 'canonical_wp_2y_63_cost4_to31',
           'native_filter_bookkeeping': False, 'bcrypt_version': '0.19.3',
           'source_sha256': '3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777'}
HOOKS = ('wp_hash_password_algorithm', 'wp_hash_password_options', 'password_needs_rehash')
