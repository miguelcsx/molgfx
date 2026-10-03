"""Import the core facade in a fresh process without notebook dependencies."""

import multiprocessing
import sys
import unittest
from importlib import import_module


def _assert_lazy_import() -> None:
    package = import_module("molgfx")
    assert not {"anywidget", "ipywidgets", "IPython"}.intersection(sys.modules)
    assert package.Viewer is import_module("molgfx.viewer").Viewer


class ViewerImportTests(unittest.TestCase):
    """The normal notebook dependency must not tax core-only consumers."""

    def test_import_keeps_notebook_modules_lazy(self) -> None:
        context = multiprocessing.get_context("spawn")
        process = context.Process(target=_assert_lazy_import)
        process.start()
        process.join(timeout=30)
        if process.is_alive():
            process.terminate()
            process.join()
            self.fail("core import did not finish")
        self.assertEqual(process.exitcode, 0)
        process.close()
