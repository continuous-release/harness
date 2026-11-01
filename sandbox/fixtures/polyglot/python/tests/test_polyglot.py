import unittest

from polyglot import describe


class PolyglotTest(unittest.TestCase):
    def test_describes_the_contract(self) -> None:
        self.assertIn("hello from polyglot", describe())
        self.assertTrue(describe().startswith("polyglot "))


if __name__ == "__main__":
    unittest.main()
