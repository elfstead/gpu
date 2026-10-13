"""The optional full-dataset handoff must never skip either input digest."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("handoff",Path(__file__).parent / "sdk/ggml/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


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


if __name__ == "__main__": unittest.main()
