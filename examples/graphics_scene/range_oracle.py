"""Pure range/count/DrawIndex expectations, usable outside the checkout."""
import struct
import oracle

STRATEGIES=('single','multi','count')


def require(value,message):
    if not value:raise ValueError(message)


def frames(capacity):
    counts=(capacity,0,1,capacity//2,capacity+7,capacity,0,1)
    return [dict(frame=f,phase=f%2,active=n,capacity=capacity) for f,n in enumerate(counts)]


def mesh_expected(frame,strategy):
    phase,active,capacity=(frame[k] for k in ('phase','active','capacity'))
    records=b''.join(struct.pack('<IIIiI',6 if strategy=='count' or i<active else 0,1,2,
                                3 if capacity>1 and i==capacity-1 else -1,0) for i in range(capacity))
    return oracle.geometry(phase,False)[:416]+oracle.GUARD+records+struct.pack('<I',active)+oracle.GUARD


def identity_image(reference,width,height,capacity,strategy):
    """Recolor accepted coverage; preserve every depth and guard byte."""
    require(len(reference)==width*height*8+256,'wrong identity reference size')
    require(capacity in (1,64,512) and strategy in STRATEGIES,'invalid identity case')
    def color(draw,surface):
        value=2*(draw+1)+surface
        return bytes((value&255,(value>>8)&255,255,255))
    colors={oracle.BLACK:oracle.BLACK,oracle.RED:color(0,0),
            oracle.GREEN:color(0 if strategy=='single' else capacity-1,1)}
    result=bytearray(reference)
    for offset in range(64,64+width*height*4,4):
        require(reference[offset:offset+4] in colors,'unexpected reference color')
        result[offset:offset+4]=colors[reference[offset:offset+4]]
    return bytes(result)
