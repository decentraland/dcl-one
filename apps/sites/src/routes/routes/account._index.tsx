import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";

import { breakpoints } from "@dcl/one-ui/foundations";

/**
 * `/account`: on a phone the layout shows the section menu on its own. From 1024px the menu is a rail
 * beside a section, so the settings open on notifications.
 */
export default function AccountIndex() {
  const navigate = useNavigate();
  const { search, state } = useLocation();

  useEffect(() => {
    if (window.matchMedia(`(min-width: ${breakpoints.desktop}px)`).matches) {
      void navigate(`/account/notifications${search}`, { replace: true, state });
    }
  }, [navigate, search, state]);

  return null;
}
