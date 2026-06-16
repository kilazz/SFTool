import struct
import sys


def detect_format(data):
    if len(data) < 8:
        return "string_table", 0, 0

    count = struct.unpack_from("<I", data, 0)[0]
    if count == 0:
        return "string_table", 0, 0

    # 1. Try string_table format (Format A)
    offset = 4
    success_a = True
    for _ in range(count):
        if offset + 5 > len(data):
            success_a = False
            break
        offset += 1  # Skip 1 byte flag
        key_len = struct.unpack_from("<I", data, offset)[0]
        offset += 4 + key_len
        if offset + 4 > len(data):
            success_a = False
            break
        text_len = struct.unpack_from("<I", data, offset)[0]
        offset += 4 + text_len * 2

    if success_a and offset == len(data):
        return "string_table", 0, 0

    # 2. Try table_based format (Format B) with N strings and E padding bytes
    for E in range(17):
        for N in range(1, 6):
            offset = 4
            success_b = True
            for _ in range(count):
                if offset + 4 + E > len(data):
                    success_b = False
                    break
                offset += 4 + E  # ID + padding
                for _ in range(N):
                    if offset + 4 > len(data):
                        success_b = False
                        break
                    str_len = struct.unpack_from("<I", data, offset)[0]
                    offset += 4 + str_len * 2

            if success_b and offset == len(data):
                return "table_based", N, E

    return "string_table", 0, 0


def debug_chunk(chunk_path):
    print(f"[*] Analyzing file: {chunk_path}")
    with open(chunk_path, "rb") as f:
        data = f.read()

    if len(data) < 4:
        print("[!] Error: File is too small!")
        return

    count = struct.unpack_from("<I", data, 0)[0]
    print(f"Header entries count: {count}")
    print(f"File size: {len(data)} bytes")

    fmt, num_strings, extra_bytes = detect_format(data)
    print(
        f"[*] Auto-detected format: {fmt} (strings per entry: {num_strings}, padding bytes: {extra_bytes})"
    )

    offset = 4
    for i in range(count):
        start_offset = offset
        if fmt == "string_table":
            if offset + 1 > len(data):
                break
            offset += 1

            if offset + 4 > len(data):
                break
            key_len = struct.unpack_from("<I", data, offset)[0]
            offset += 4

            if offset + key_len > len(data):
                break
            key = data[offset : offset + key_len].decode("utf-8", errors="ignore")
            offset += key_len

            if offset + 4 > len(data):
                break
            text_len = struct.unpack_from("<I", data, offset)[0]
            offset += 4

            if text_len * 2 > len(data) - offset:
                break
            text = data[offset : offset + (text_len * 2)].decode(
                "utf-16-le", errors="ignore"
            )
            offset += text_len * 2

            if i < 5 or i >= count - 5 or i % 100 == 0:
                print(
                    f"[{i:4d}] Offset: {start_offset:<6d} | Key: {repr(key):<20} | Text: {repr(text[:40])}"
                )
        else:
            # Read table_based format with dynamic strings count N and padding E
            if offset + 4 > len(data):
                break
            id_val = struct.unpack_from("<I", data, offset)[0]
            offset += 4

            param_bytes = b""
            if extra_bytes > 0:
                param_bytes = data[offset : offset + extra_bytes]
                offset += extra_bytes

            strings = []
            for _ in range(num_strings):
                if offset + 4 > len(data):
                    break
                str_len = struct.unpack_from("<I", data, offset)[0]
                offset += 4

                if str_len * 2 > len(data) - offset:
                    break
                s_text = data[offset : offset + str_len * 2].decode(
                    "utf-16-le", errors="ignore"
                )
                offset += str_len * 2
                strings.append(s_text)

            if i < 5 or i >= count - 5 or i % 100 == 0:
                str_previews = " | ".join(
                    [f"Str{idx}: {repr(s[:20])}" for idx, s in enumerate(strings)]
                )
                print(
                    f"[{i:4d}] Offset: {start_offset:<6d} | ID: {id_val:<5d} | Params: {param_bytes.hex()} | {str_previews}"
                )

    print(f"[*] Processing completed successfully at offset {offset}/{len(data)}")


if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "work_folder/chunk_12.dat"
    debug_chunk(path)
