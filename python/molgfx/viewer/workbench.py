"""A viewer with a command console, driven by one authoring session."""

from .viewer import Viewer

import traitlets


def _error_records(error):
    """The located errors a ``CommandError`` carries, as plain dictionaries."""
    records = getattr(error, "errors", None)
    if records:
        return [dict(record) for record in records]
    return [{"kind": "scene", "message": str(error)}]


class Workbench(Viewer):
    """Canvas, command line, history and errors over one live session.

    Commands typed in the page, or passed to :meth:`execute`, run in the kernel
    against ``session``; the scene publishes each resulting patch to the page
    exactly as it does for any other edit, so the page receives semantic edits
    and never pixels or a second copy of the structure.

    ```python
    import molframe, molgfx as mg
    from molgfx.viewer import Workbench

    bench = Workbench(molframe.read("4hhb.cif"))
    bench.execute("show cartoon, protein; select pocket, byres (within 5 of resname HEM)")
    bench
    ```
    """

    workbench = traitlets.Bool(True).tag(sync=True)
    history = traitlets.List(traitlets.Unicode()).tag(sync=True)
    command_request = traitlets.Dict().tag(sync=True)
    command_reply = traitlets.Dict().tag(sync=True)

    def __init__(self, source, **kwargs):
        """Wrap a ``Session``, or start one over a ``Scene`` or a structure."""
        from .. import Session

        self.session = source if isinstance(source, Session) else Session(source)
        super().__init__(self.session.scene, **kwargs)
        self.history = list(self.session.history)
        self.observe(self._on_request, names="command_request")

    @property
    def scene(self):
        """The live scene the session edits."""
        return self.session.scene

    def execute(self, text):
        """Run command text as one atomic edit and return the ``CommandResult``.

        Errors raise ``CommandError``, exactly as ``Session.execute`` does.
        """
        result = self.session.execute(text)
        self.history = list(self.session.history)
        return result

    def _reply(self, request):
        """The answer to one request from the page."""
        kind = request.get("type")
        identity = request.get("id")
        if kind == "complete":
            text = str(request.get("text", ""))
            cursor = request.get("cursor")
            completions = self.session.completions(
                text, cursor if isinstance(cursor, int) else None
            )
            return {
                "type": "completions",
                "id": identity,
                "items": [
                    {"text": text, "kind": kind, "detail": detail}
                    for text, kind, detail in completions
                ],
            }
        if kind == "execute":
            text = str(request.get("text", ""))
            try:
                result = self.execute(text)
            except Exception as error:  # every failure is reported to the page
                return {
                    "type": "result",
                    "id": identity,
                    "ok": False,
                    "text": text,
                    "errors": _error_records(error),
                    "rendered": str(error),
                    "history": list(self.history),
                }
            return {
                "type": "result",
                "id": identity,
                "ok": True,
                "text": text,
                "revision": result.revision,
                "messages": list(result.messages),
                "history": list(self.history),
            }
        return {"type": "error", "id": identity, "message": f"unknown request {kind!r}"}

    def _on_request(self, change):
        """Answer a request the page wrote into ``command_request``."""
        request = change["new"]
        if isinstance(request, dict) and request:
            self.command_reply = self._reply(request)
