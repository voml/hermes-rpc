const crypto = require('crypto');
const secret = process.env.NPM_TOTP_SECRET || process.argv[2];
if (!secret) { console.error('no secret'); process.exit(1); }

function base32Decode(s) {
    const A = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
    let bits = '';
    for (const c of s.toUpperCase()) {
        const i = A.indexOf(c);
        if (i >= 0) bits += i.toString(2).padStart(5, '0');
    }
    const bytes = Buffer.alloc(Math.floor(bits.length / 8));
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(bits.substring(i*8, i*8+8), 2);
    return bytes;
}
const counter = Math.floor(Date.now() / 30000);
const key = base32Decode(secret);
const buf = Buffer.alloc(8);
buf.writeBigInt64BE(BigInt(counter), 0);
const hmac = crypto.createHmac('sha1', key).update(buf).digest();
const offset = hmac[hmac.length - 1] & 0xf;
const code = ((hmac[offset] & 0x7f) << 24) | ((hmac[offset+1] & 0xff) << 16) | ((hmac[offset+2] & 0xff) << 8) | (hmac[offset+3] & 0xff);
console.log((code % 1000000).toString().padStart(6, '0'));
