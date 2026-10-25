import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
class Tests(unittest.TestCase):
    def run_cli(self,args,text='',cwd=None):
        env={**os.environ,'PYTHONPATH':str(Path(__file__).resolve().parent.parent)}
        return subprocess.run([sys.executable,'-m','hard_cli.cli',*args],input=text,text=True,capture_output=True,cwd=cwd,env=env,timeout=3)
    def success(self,result,want):
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(result.stderr,'')
        self.assertEqual(json.loads(result.stdout),want)
    def test_default_stdin_and_empty(self):
        self.success(self.run_cli([],' a \nβ\n'),[' a ','β'])
        self.success(self.run_cli([],''),[])
        self.success(self.run_cli(['--limit','0'],'x\n'),[])
    def test_stream_order_limit_and_dash_filename(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d,'one').write_text('first\n')
            Path(d,'-last').write_text('last\n')
            self.success(self.run_cli(['--limit','3','--','one','-','-last'],'middle\nextra\n',d),['first','middle','extra'])
            self.success(self.run_cli(['--','-last'],'ignored',d),['last'])
            self.success(self.run_cli(['one','-'],'middle\n',d),['first','middle'])
    def test_errors_are_atomic_usage_errors(self):
        for args in [['-','-'],['--limit','-1'],['--limit','no'],['--unknown'],['missing-file']]:
            result=self.run_cli(args,'must not leak\n')
            self.assertEqual(result.returncode,2,result.stderr)
            self.assertEqual(result.stdout,'')
            self.assertIn('error:',result.stderr)
            self.assertNotIn('Traceback',result.stderr)
