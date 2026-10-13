"""The optional full-dataset handoff must never skip either input digest."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("handoff",Path(__file__).parent / "sdk/ggml/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)
spec = importlib.util.spec_from_file_location("measure",Path(__file__).parent / "sdk/ggml/measure.py")
measure = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules,{'run':handoff}): spec.loader.exec_module(measure)


class DigestTests(unittest.TestCase):
    def test_exact_dataset(self):
        with tempfile.TemporaryDirectory() as directory:
            data = Path(directory)
            files = [data / name for name in ("t10k-images-idx3-ubyte","t10k-labels-idx1-ubyte")]
            for index,path in enumerate(files): path.write_bytes(bytes([index]))
            rows = [handoff.digest(p)+"  "+p.name for p in files]
            handoff.verify_data(data,"\n".join(rows))
            for invalid in ("",rows[0],rows[1],"\n".join([*rows,rows[0]]),"\n".join(rows).replace(rows[0][:64],"0"*64)):
                with self.assertRaises(ValueError): handoff.verify_data(data,invalid)
            files[1].write_bytes(b"changed")
            with self.assertRaises(ValueError): handoff.verify_data(data,"\n".join(rows))


class TransformerReceiptTests(unittest.TestCase):
    def test_complete_and_malformed(self):
        lines=[]
        for d,h,t in ((1,1,1),(13,29,3),(13,29,17),(64,128,16),(65,129,17)):
            for precision in ('f32','f16'):
                base=f'd={d} h={h} t={t} precision={precision}'
                lines.extend(f'transformer_error {base} tensor={name} max_abs=0' for name in
                    ('N','Q','K','V','scores','P','Vt','A','O','R','NR','up','relu','down','Y'))
                lines.append(f'transformer {base} phases=3 dispatches=45 rejected=11 retry=15 allocation_bytes=1024 PASS')
            lines.append(f'transformer_weight_drift d={d} h={h} t={t} max_abs=0.001')
        lines.append('Transformer profile placement=device cases=10 PASS; no timing claim')
        valid='\n'.join(lines)
        self.assertEqual(len(handoff.verify_transformer(valid,'device')['cases']),10)
        for bad in ('',valid.replace('tensor=Y','tensor=N'),valid.replace('max_abs=0','max_abs=nan',1),
                    valid.replace('rejected=11','rejected=10',1),valid.replace('dispatches=45','dispatches=44',1),
                    valid.replace('precision=f16','precision=f32'),valid.replace('placement=device','placement=host'),
                    '\n'.join(lines[1:]),valid+'\n'+lines[-1]):
            with self.subTest(bad=bad[:80]):
                with self.assertRaises(ValueError):handoff.verify_transformer(bad,'device')


class QuantizedReceiptTests(unittest.TestCase):
    def test_complete_and_malformed(self):
        lines=[]
        for k,m,n in ((32,1,1),(32,11,3),(64,13,17),(96,17,5),(256,128,64)):
            for p in ('f32','f16','q8'):
                size=k*m*4 if p=='f32' else k*m*2 if p=='f16' else k*m//32*34
                lines.append(f'quantized_linear k={k} m={m} n={n} precision={p} phases=3 dispatches=3 rejected=10 retry=1 '
                             f'max_error=0 max_drift=0 error_bound_ratio=0 allocation_bytes={size+1024} weight_bytes={size} PASS')
        lines.append('Quantized linear profile placement=device cases=15 PASS; no acceleration claim')
        valid='\n'.join(lines)
        self.assertEqual(len(handoff.verify_quantized(valid,'device')),15)
        for bad in ('',valid.replace('precision=q8','precision=f32'),valid.replace('error_bound_ratio=0','error_bound_ratio=1.01',1),
                    valid.replace('max_error=0','max_error=nan',1),valid.replace('weight_bytes=128','weight_bytes=127',1),
                    valid.replace('rejected=10','rejected=9',1),valid.replace('placement=device','placement=host'),
                    '\n'.join(lines[1:]),valid+'\n'+lines[-1]):
            with self.subTest(bad=bad[:80]):
                with self.assertRaises(ValueError):handoff.verify_quantized(bad,'device')


class MeasurementTests(unittest.TestCase):
    def test_complete_and_malformed(self):
        for kind,shapes,precisions in (
            ('transformer',((1,1,1),(13,29,3),(13,29,17),(64,128,16),(65,129,17)),('f32','f16')),
            ('linear',((32,1,1),(32,11,3),(64,13,17),(96,17,5),(256,128,64)),('f32','f16','q8'))):
            lines=[]
            keys=('d','h','t') if kind=='transformer' else ('k','m','n')
            for shape in shapes:
                for precision in precisions:
                    identity=f'kind={kind} '+ ' '.join(f'{key}={value}' for key,value in zip(keys,shape))+f' precision={precision}'
                    lines.extend([f'ml_samples {identity} per_graph_ms=1,2,3,4,5,6,7,8,9',
                        f'ml_timing {identity} warmups=2 samples=9 iterations=10 min_ms=1 median_ms=5 max_ms=9 items_per_s={shape[2]*200} allocation_bytes=2000 weight_bytes=1000'])
            valid='\n'.join(lines)
            self.assertEqual(len(measure.verify_timing(valid,kind)),len(shapes)*len(precisions))
            for bad in ('',valid.replace('median_ms=5','median_ms=4',1),valid.replace('items_per_s=200','items_per_s=201',1),
                        valid.replace('1,2,3,4,5,6,7,8,9','1,2,3,4,5,6,7,8,nan',1),
                        valid.replace('samples=9','samples=8',1),'\n'.join(lines[1:]),valid+'\n'+lines[-1]):
                with self.subTest(kind=kind,bad=bad[:80]):
                    with self.assertRaises(ValueError):measure.verify_timing(bad,kind)


if __name__ == "__main__": unittest.main()
