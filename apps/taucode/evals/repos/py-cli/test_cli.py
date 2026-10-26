import unittest
import ast
import inspect
import cli

class Tests(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(cli.parse(' a,, b '),['a','b'])
    def test_average(self):
        self.assertEqual(cli.average([]),0); self.assertEqual(cli.average([2,4]),3)
    def test_truth(self):
        self.assertFalse(cli.truth('false')); self.assertTrue(cli.truth('YES'))
    def test_extension(self):
        self.assertEqual(cli.extension('a.TXT'),'.txt'); self.assertEqual(cli.extension('a'),'')
    def test_frequency(self):
        self.assertEqual(cli.frequency(['a','b','a']),{'a':2,'b':1})
    def test_total(self):
        self.assertEqual(cli.total([2,3]),5); self.assertNotIn("ListComp", ast.dump(ast.parse(inspect.getsource(cli.total))))
