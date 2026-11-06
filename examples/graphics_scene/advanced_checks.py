"""Compare different triangulations without claiming identical interpolation at ties."""
import math
import struct
import textured_oracle as texture
from textured_checks import require


def compare_triangulations(baseline, output, width, height):
    size = width*height*4
    maximum = interior_maximum = changed_boundary_pixels = 0
    for mode in range(6):
        for frame in range(3):
            stem = f'mode-{mode}-frame-{frame}'
            for suffix in ('geometry', 'texture'):
                require((baseline/(stem+'.'+suffix)).read_bytes()==(output/(stem+'.'+suffix)).read_bytes(),
                        'stage composition changed generated input')
            a=(baseline/(stem+'.images')).read_bytes()
            b=(output/(stem+'.images')).read_bytes()
            require(len(a)==len(b)==2*size+256, 'stage comparison extent mismatch')
            require(a[:64]==b[:64]==texture.GUARD and a[64+size:192+size]==b[64+size:192+size]==texture.GUARD*2
                    and a[-64:]==b[-64:]==texture.GUARD, 'stage comparison guard mismatch')
            for pixel in range(width*height):
                start=64+pixel*4
                delta=max(abs(x-y) for x,y in zip(a[start:start+4],b[start:start+4]))
                maximum=max(maximum,delta)
                # Linear filtering has no nearest-texel tie. Require every pixel.
                if texture.CASES[mode][2] or texture.interior(pixel%width,pixel//width,width,height,int(frame==1),mode):
                    interior_maximum=max(interior_maximum,delta)
                    require(delta<=2, 'stage composition interior/linear color mismatch')
                elif delta>2:
                    changed_boundary_pixels+=1
            require(all(math.isfinite(x[0]) and math.isfinite(y[0]) and abs(x[0]-y[0])<=1e-6
                        for x,y in zip(struct.iter_unpack('<f',a[192+size:-64]),struct.iter_unpack('<f',b[192+size:-64]))),
                    'stage composition depth mismatch')
    return dict(frames=18,pixels=18*width*height,max_color_byte_delta=maximum,
                max_checked_color_byte_delta=interior_maximum,boundary_pixels_over_tolerance=changed_boundary_pixels,
                boundary_policy='existing CPU oracle polygon-edge/nearest-texel tie mask; linear color and all depth unmasked')
