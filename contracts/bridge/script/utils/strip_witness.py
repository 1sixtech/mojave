#!/usr/bin/env python3
"""
Strip witness data from a SegWit transaction to get the witness-stripped TXID.
"""

import sys

def strip_witness_data(rawtx_hex):
    """
    Strip witness data from a SegWit transaction.
    
    SegWit transaction format:
    - Version (4 bytes)
    - Marker (1 byte) = 0x00
    - Flag (1 byte) = 0x01
    - Input count (varint)
    - Inputs...
    - Output count (varint)
    - Outputs...
    - Witness data...
    - Locktime (4 bytes)
    
    Legacy transaction format (what we need):
    - Version (4 bytes)
    - Input count (varint)
    - Inputs...
    - Output count (varint)
    - Outputs...
    - Locktime (4 bytes)
    """
    raw = bytes.fromhex(rawtx_hex.replace('0x', ''))
    
    # Check for SegWit marker (0x0001 after version)
    if len(raw) < 6 or raw[4:6] != bytes([0x00, 0x01]):
        print("Not a SegWit transaction, returning as-is")
        return rawtx_hex
    
    print("SegWit transaction detected")
    
    # Parse transaction
    pos = 0
    
    # Version (4 bytes)
    version = raw[pos:pos+4]
    pos += 4
    
    # Skip marker + flag
    marker_flag = raw[pos:pos+2]
    pos += 2
    print(f"Marker+Flag: {marker_flag.hex()}")
    
    # Input count (varint)
    input_count, varint_size = read_varint(raw, pos)
    pos += varint_size
    print(f"Input count: {input_count}")
    
    # Parse inputs (before witness)
    inputs_start = pos - varint_size
    for i in range(input_count):
        # Previous output (32 + 4 bytes)
        pos += 36
        # Script length
        script_len, varint_size = read_varint(raw, pos)
        pos += varint_size
        # Script
        pos += script_len
        # Sequence (4 bytes)
        pos += 4
    inputs_data = raw[inputs_start:pos]
    
    # Output count and outputs
    outputs_start = pos
    output_count, varint_size = read_varint(raw, pos)
    pos += varint_size
    print(f"Output count: {output_count}")
    
    for i in range(output_count):
        # Value (8 bytes)
        pos += 8
        # Script length
        script_len, varint_size = read_varint(raw, pos)
        pos += varint_size
        # Script
        pos += script_len
    outputs_data = raw[outputs_start:pos]
    
    # Skip witness data (everything before locktime)
    # Locktime is last 4 bytes
    locktime = raw[-4:]
    
    # Construct legacy format
    legacy_tx = version + inputs_data + outputs_data + locktime
    
    print(f"Original length: {len(raw)} bytes")
    print(f"Stripped length: {len(legacy_tx)} bytes")
    
    return '0x' + legacy_tx.hex()

def read_varint(data, pos):
    """Read a Bitcoin varint."""
    first_byte = data[pos]
    if first_byte < 0xfd:
        return first_byte, 1
    elif first_byte == 0xfd:
        return int.from_bytes(data[pos+1:pos+3], 'little'), 3
    elif first_byte == 0xfe:
        return int.from_bytes(data[pos+1:pos+5], 'little'), 5
    else:  # 0xff
        return int.from_bytes(data[pos+1:pos+9], 'little'), 9

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: strip_witness.py <raw_tx_hex>")
        sys.exit(1)
    
    rawtx = sys.argv[1]
    stripped = strip_witness_data(rawtx)
    print(f"\nWitness-stripped TX:")
    print(stripped)
