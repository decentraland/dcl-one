// A consumer of the library: the public entry, the foundations stylesheet and every promoted
// component (preview-only ones by path), so a real build resolves and processes all their CSS.
import '../../foundations/foundations.css'
import * as ui from '../../index'
import { NavBar } from '../../components/NavBar/NavBar'

export { NavBar, ui }
