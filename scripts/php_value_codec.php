<?php
/** Development-only byte/value wire codec; no production ABI is specified. */
function decode_integer_wire($value) {
    if (PHP_INT_SIZE !== 8 || !is_string($value) || $value === '') {
        throw new RuntimeException('integer wire requires a signed 64-bit decimal string');
    }
    $negative = $value[0] === '-';
    $offset = ($negative || $value[0] === '+') ? 1 : 0;
    $digits = substr($value, $offset);
    if ($digits === '' || strspn($digits, '0123456789') !== strlen($digits)) {
        throw new RuntimeException('integer wire contains non-decimal bytes');
    }
    $significant = ltrim($digits, '0');
    if ($significant === '') {
        return 0;
    }
    $limit = $negative ? '9223372036854775808' : '9223372036854775807';
    if (strlen($significant) > strlen($limit)
        || (strlen($significant) === strlen($limit) && strcmp($significant, $limit) > 0)) {
        throw new RuntimeException('integer wire is outside the signed 64-bit range');
    }
    // Cast only an already-validated canonical decimal. PHP's permissive raw
    // cast can otherwise clamp, discard junk or turn very long integers to 0.
    return (int) (($negative ? '-' : '') . $significant);
}

function decode_value($input) {
    return match ($input['type']) {
        'null' => null,
        'bool' => $input['value'],
        'int' => decode_integer_wire($input['value'] ?? null),
        'float' => match ($input['value']) { 'nan' => NAN, 'inf' => INF, '-inf' => -INF, default => (float) $input['value'] },
        'string' => hex2bin($input['hex']),
        'array' => decode_array($input['entries']),
        default => throw new RuntimeException('unsupported oracle input'),
    };
}

function decode_array($entries) {
    $array = [];
    foreach ($entries as $entry) {
        $key = decode_value($entry['key']);
        $array[$key] = decode_value($entry['value']);
    }
    return $array;
}

function encode_value($value) {
    if (is_null($value)) return ['type' => 'null'];
    if (is_bool($value)) return ['type' => 'bool', 'value' => $value];
    if (is_int($value)) return ['type' => 'int', 'value' => (string) $value];
    if (is_float($value)) return ['type' => 'float', 'bits' => is_nan($value) ? 'nan' : bin2hex(pack('E', $value))];
    if (is_string($value)) return ['type' => 'string', 'hex' => bin2hex($value)];
    if (is_array($value)) {
        $entries = [];
        foreach ($value as $key => $item) $entries[] = ['key' => encode_value($key), 'value' => encode_value($item)];
        return ['type' => 'array', 'entries' => $entries];
    }
    throw new RuntimeException('unsupported encoded output');
}

