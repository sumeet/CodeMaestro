#!/usr/bin/env python3
# this was used to generate the rgba for editor/winicon.bin

from PIL import Image
from io import BytesIO
import itertools

imgobj = Image.open('./loading.png').convert('RGBA')
raw_rgba = itertools.chain(*imgobj.getdata())
open('winicon.bin', 'wb').write(bytes(raw_rgba))
