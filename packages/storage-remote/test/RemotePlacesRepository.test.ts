import { InMemoryPlacesRepository } from "@webglobe/core";
import { sharedRepositoryContractTests } from "@webglobe/core/src/storage/repositoryContractTests.js";
import { RemotePlacesRepository } from "../src/RemotePlacesRepository.js";

sharedRepositoryContractTests(() => new RemotePlacesRepository(new InMemoryPlacesRepository()));
