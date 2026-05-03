import { useEffect } from "react";
import "./App.css";
import { ControlPane } from "./components/ControlPane";
import { ImportPane } from "./components/ImportPane";
import { PreviewPane } from "./components/PreviewPane";
import { useEditorStore } from "./stores/editorStore";
import { useExportStore } from "./stores/exportStore";
import { useLibraryStore } from "./stores/libraryStore";
import { useTemplateStore } from "./stores/templateStore";

function App() {
  const library = useLibraryStore();
  const editor = useEditorStore();
  const exportStore = useExportStore();
  const templateStore = useTemplateStore();

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    if (new URLSearchParams(window.location.search).get("qa") !== "watermark") {
      return;
    }

    library.addImportResult({
      accepted: [
        {
          byteSize: 1600000,
          extension: "png",
          filename: "0_0-7.png",
          height: 1456,
          id: "qa-watermark-sample-1",
          path: "qa-watermark-sample-1",
          previewSrc: "/qa-watermark-sample.svg",
          status: "ready",
          width: 816,
        },
        {
          byteSize: 250000,
          extension: "jpeg",
          filename:
            "wallzzz007_A_green_apple_falls_into_green_water_dynamic_clean_a13bb084-f064-4596-8f4d-97f6d05cf489_2.jpeg",
          height: 1456,
          id: "qa-watermark-sample-2",
          path: "qa-watermark-sample-2",
          previewSrc: "/qa-watermark-sample.svg",
          status: "ready",
          width: 816,
        },
        {
          byteSize: 1600000,
          extension: "png",
          filename:
            "wallzzz007_httpss.mj.runXpQ4w9lG4WE_fresh_cherry_tomatoes_spl_daa8004b-2d89-443f-8d61-7c43d12aad61_3.png",
          height: 1456,
          id: "qa-watermark-sample-3",
          path: "qa-watermark-sample-3",
          previewSrc: "/qa-watermark-sample.svg",
          status: "ready",
          width: 816,
        },
      ],
      skipped: [],
    });
  }, []);

  return (
    <div className={`app-shell ${exportStore.isExporting ? "exporting" : ""}`}>
      <ImportPane exportResults={exportStore.results} library={library} />
      <PreviewPane
        asset={library.selectedAsset}
        editor={editor}
        isImporting={library.isImporting}
      />
      <ControlPane
        editor={editor}
        exportStore={exportStore}
        library={library}
        templateStore={templateStore}
      />
    </div>
  );
}

export default App;
