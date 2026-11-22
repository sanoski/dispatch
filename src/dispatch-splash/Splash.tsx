// Dispatch: the splash screen (ARTWORK.md, "Splash screen"). Joshua's art
// with the wordmark and version drawn on top in code, so they stay sharp
// and follow the version number. No translated text: the brand name is the
// same in every language, so this window skips loading i18n.
import React, { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { DispatchIcon } from "@/components/icons/DispatchLogo";
import art from "@/assets/dispatch/art/splash.webp";
import "./Splash.css";

const BRAND_NAME = "Dispatch";

export const Splash: React.FC = () => {
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  return (
    <div className="ds-splash">
      <img className="ds-art" src={art} alt="" draggable={false} />
      <div className="ds-shade" aria-hidden="true" />
      <div className="ds-brand">
        <DispatchIcon size={52} />
        <div className="ds-words">
          <span className="ds-name">{BRAND_NAME}</span>
          {version && <span className="ds-version">{`v${version}`}</span>}
        </div>
      </div>
      <div className="ds-loading" aria-hidden="true">
        <span />
      </div>
    </div>
  );
};
