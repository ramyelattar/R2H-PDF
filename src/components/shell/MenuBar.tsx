import { useState } from "react";
import "./MenuBar.css";

interface MenuBarProps {
  onOpenFile: () => void;
  onOpenRecent: () => void;
  onOpenBentoPdf: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onExport: () => void;
  canExport: boolean;
  onCloseDocument: () => void;
  onExit: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onFind: () => void;
  onPreferences: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFitPage: () => void;
  onFitWidth: () => void;
  onToggleLeftSidebar: () => void;
  onToggleRightSidebar: () => void;
  onFullScreen: () => void;
}

export const MenuBar = ({
  onOpenFile,
  onOpenRecent,
  onOpenBentoPdf,
  onSave,
  onSaveAs,
  onExport,
  canExport,
  onCloseDocument,
  onExit,
  onUndo,
  onRedo,
  onCut,
  onCopy,
  onPaste,
  onFind,
  onPreferences,
  onZoomIn,
  onZoomOut,
  onFitPage,
  onFitWidth,
  onToggleLeftSidebar,
  onToggleRightSidebar,
  onFullScreen,
}: MenuBarProps) => {
  const [openMenu, setOpenMenu] = useState<"file" | "edit" | "view" | null>(
    null
  );

  const handleMenuClick = (menu: "file" | "edit" | "view") => {
    setOpenMenu(openMenu === menu ? null : menu);
  };

  const handleMenuItemClick = (callback: () => void) => {
    callback();
    setOpenMenu(null);
  };

  const handleClickOutside = () => {
    setOpenMenu(null);
  };

  return (
    <>
      <nav className="menu-bar">
        <div className="menu-bar__item">
          <button
            className={`menu-bar__trigger ${
              openMenu === "file" ? "menu-bar__trigger--active" : ""
            }`}
            onClick={() => handleMenuClick("file")}
          >
            File
          </button>
          {openMenu === "file" && (
            <div className="menu-bar__dropdown">
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onOpenFile)}
              >
                Open
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onOpenRecent)}
              >
                Open Recent
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onOpenBentoPdf)}
                data-testid="menu-open-bentopdf"
              >
                BentoPDF Tools
              </button>
              <div className="menu-bar__divider" />              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onSave)}
              >
                Save
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onSaveAs)}
              >
                Save As
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onExport)}
                disabled={!canExport}
                aria-disabled={!canExport}
                title={canExport ? "Open the PDF export workflow" : "Open a PDF before exporting"}
              >
                Export
              </button>
              <div className="menu-bar__divider" />
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onCloseDocument)}
              >
                Close Document
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onExit)}
              >
                Exit
              </button>
            </div>
          )}
        </div>

        <div className="menu-bar__item">
          <button
            className={`menu-bar__trigger ${
              openMenu === "edit" ? "menu-bar__trigger--active" : ""
            }`}
            onClick={() => handleMenuClick("edit")}
          >
            Edit
          </button>
          {openMenu === "edit" && (
            <div className="menu-bar__dropdown">
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onUndo)}
              >
                Undo
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onRedo)}
              >
                Redo
              </button>
              <div className="menu-bar__divider" />
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onCut)}
              >
                Cut
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onCopy)}
              >
                Copy
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onPaste)}
              >
                Paste
              </button>
              <div className="menu-bar__divider" />
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onFind)}
              >
                Find
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onPreferences)}
              >
                Preferences
              </button>
            </div>
          )}
        </div>

        <div className="menu-bar__item">
          <button
            className={`menu-bar__trigger ${
              openMenu === "view" ? "menu-bar__trigger--active" : ""
            }`}
            onClick={() => handleMenuClick("view")}
          >
            View
          </button>
          {openMenu === "view" && (
            <div className="menu-bar__dropdown">
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onZoomIn)}
              >
                Zoom In
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onZoomOut)}
              >
                Zoom Out
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onFitPage)}
              >
                Fit Page
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onFitWidth)}
              >
                Fit Width
              </button>
              <div className="menu-bar__divider" />
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onToggleLeftSidebar)}
              >
                Toggle Left Sidebar
              </button>
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onToggleRightSidebar)}
              >
                Toggle Right Sidebar
              </button>
              <div className="menu-bar__divider" />
              <button
                className="menu-bar__menu-item"
                onClick={() => handleMenuItemClick(onFullScreen)}
              >
                Full Screen
              </button>
            </div>
          )}
        </div>
      </nav>

      {/* Click-outside overlay */}
      {openMenu && (
        <div
          className="menu-bar__overlay"
          onClick={handleClickOutside}
          onMouseDown={handleClickOutside}
        />
      )}
    </>
  );
};
