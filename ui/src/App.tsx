import { BrowserRouter as Router, Routes, Route, Link } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
//TODO

function SingleImage() {
  return (
    <div className="page-container">
      <h2>Single Image Scrambling</h2>
      <p>Upload and scramble a single image.</p>
      {/* TODO */}
    </div>
  );
}
//TODO
function FolderProcess() {
  return (
    <div className="page-container">
      <h2>Batch Image Scrambling</h2>
      <p>Process multiple images from a folder.</p>
      {/* TODO */}
    </div>
  );
}
//TODO

function VideoProcess() {
  return (
    <div className="page-container">
      <h2>Video Scrambling</h2>
      <p>Upload and scramble video content.</p>
      {/* TODO */}
    </div>
  );
}

function MainMenu() {
  return (
    <div className="menu-container">
      <h1>Image Scrambling Tool</h1>
      <p>Select processing type:</p>
      <div className="button-container">
        <Link to="/single" className="menu-button">
          Single Image Processing
        </Link>
        <Link to="/folder" className="menu-button">
          Multiple Image Processing
        </Link>
        <Link to="/video" className="menu-button">
          Video Processing
        </Link>
      </div>
    </div>
  );
}

function App() {
  return (
    <Router>
      <div className="container">
        <Routes>
          <Route path="/" element={<MainMenu />} />
          <Route path="/single" element={<SingleImage />} />
          <Route path="/folder" element={<FolderProcess />} />
          <Route path="/video" element={<VideoProcess />} />
        </Routes>
      </div>
    </Router>
  );
}

export default App;