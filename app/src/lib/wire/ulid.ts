// ULIDs as 26-character Crockford base32 strings (the daemon's JSON form)
// and 16 raw bytes (the data-frame form).
const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export function ulidToBytes(id: string): Uint8Array {
  if (id.length !== 26) throw new Error(`ULID must have 26 characters: ${id}`);
  let value = 0n;
  for (const ch of id.toUpperCase()) {
    const digit = ALPHABET.indexOf(ch);
    if (digit < 0) throw new Error(`invalid ULID character ${ch} in ${id}`);
    value = (value << 5n) | BigInt(digit);
  }
  const out = new Uint8Array(16);
  for (let i = 15; i >= 0; i--) {
    out[i] = Number(value & 0xffn);
    value >>= 8n;
  }
  return out;
}

export function bytesToUlid(bytes: Uint8Array): string {
  let value = 0n;
  for (const b of bytes) value = (value << 8n) | BigInt(b);
  let out = "";
  for (let i = 0; i < 26; i++) {
    out = ALPHABET[Number(value & 31n)] + out;
    value >>= 5n;
  }
  return out;
}

export function newUlid(): string {
  const bytes = new Uint8Array(16);
  let ms = Date.now();
  for (let i = 5; i >= 0; i--) {
    bytes[i] = ms % 256;
    ms = Math.floor(ms / 256);
  }
  crypto.getRandomValues(bytes.subarray(6));
  return bytesToUlid(bytes);
}
