import io, os, sys, unittest
from contextlib import redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
try:
    import capstone  # noqa: E402,F401
except ImportError:
    raise unittest.SkipTest('capstone is not installed')
import layoutcmp  # noqa: E402


def dis(code, at=0x1000):
    return list(layoutcmp.MD.disasm(code, at))


class Normalise(unittest.TestCase):
    def test_relocated_operands_are_wildcarded(self):
        call = dis(bytes.fromhex('E8 10 00 00 00'))[0]
        lea = dis(bytes.fromhex('48 8D 05 10 00 00 00'))[0]
        self.assertEqual(layoutcmp.norm(call), 'call T')
        self.assertEqual(layoutcmp.norm(lea), 'lea rax, [rip+R]')
        self.assertEqual(layoutcmp.pattern_of([call, lea]), 'E8 ?? ?? ?? ?? 48 8D 05 ?? ?? ?? ??')

    def test_displacements_stay(self):
        a = dis(bytes.fromhex('48 8B 88 D0 02 00 00'))[0]
        b = dis(bytes.fromhex('48 8B 88 A0 02 00 00'))[0]
        self.assertNotEqual(layoutcmp.norm(a), layoutcmp.norm(b))
        self.assertEqual(layoutcmp.disp_of(a), 0x2d0)
        self.assertFalse(layoutcmp.same_slot(a, b))

    def test_a_wide_store_covers_the_fields_inside_it(self):
        store = dis(bytes.fromhex('C5 F8 11 81 C8 19 00 00'))[0]
        self.assertTrue(layoutcmp.covers(store, 0x19c8) and layoutcmp.covers(store, 0x19d0))
        self.assertFalse(layoutcmp.covers(store, 0x19d8))


@unittest.skipUnless(os.environ.get('SWOFTY_STEAM_EXE') and os.environ.get('SWOFTY_EA_DUMP'),
                     'set SWOFTY_STEAM_EXE and SWOFTY_EA_DUMP')
class OnTheEaBuild(unittest.TestCase):
    def setUp(self):
        import bindverify
        if not bindverify.is_kb_build(os.environ['SWOFTY_STEAM_EXE']):
            self.skipTest('SWOFTY_STEAM_EXE is not the KB build')

    def run_tool(self, *fields):
        out = io.StringIO()
        args = ['--steam', os.environ['SWOFTY_STEAM_EXE'], '--other', os.environ['SWOFTY_EA_DUMP']]
        for f in fields:
            args += ['--field', f]
        with redirect_stdout(out):
            rc = layoutcmp.main(args)
        return rc, out.getvalue()

    def test_read_fields_keep_their_displacement(self):
        rc, text = self.run_tool('CarLiveInputs.throttle', 'RelativeGapEntry.leader_gap_ms', 'CarErs.energy',
                                 'SessionVehicleEntry.status', 'PenaltyRecord.penalty_type')
        self.assertEqual(rc, 0, text)
        self.assertEqual(text.count('| yes'), 5, text)

    def test_the_game_manager_progress_system_moved(self):
        rc, text = self.run_tool('GameManagerLive.progress_system')
        self.assertEqual(rc, 1)
        self.assertIn('[rax + 0x2a0]', text)
        self.assertIn('[rax + 0x2d0]', text)


if __name__ == '__main__':
    unittest.main()
